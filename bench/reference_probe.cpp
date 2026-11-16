#define main circles_contest_main
#include "../legacy/circles-hackbfgs.tr.cpp"
#undef main

#include <cstdlib>
#include <iomanip>
#include <limits>
#include <stdexcept>
#include <string>

struct ProbeInput {
  int n;
  Instance radii;
  double p1, p2;
  Vec state, direction;
  bool has_previous;
  double previous, c1, c2;
  int max_steps;
  double gradient_tol, callback_grad_tol, violation_tol, sample_grad_squared;
};

ProbeInput readProbeInput() {
  ProbeInput input{};
  if (!(std::cin >> input.n) || input.n < 1 || input.n > 50) {
    throw std::runtime_error("n must be in 1..50");
  }
  input.radii.resize(input.n);
  for (double& radius : input.radii) {
    if (!(std::cin >> radius) || !std::isfinite(radius) || radius <= 0.0)
      throw std::runtime_error("radii must be finite and positive");
  }
  if (!(std::cin >> input.p1 >> input.p2)) throw std::runtime_error("missing weights");
  input.state.resize(1 + 2 * input.n);
  input.direction.resize(input.state.size());
  for (double& value : input.state)
    if (!(std::cin >> value) || !std::isfinite(value)) throw std::runtime_error("invalid state");
  for (double& value : input.direction)
    if (!(std::cin >> value) || !std::isfinite(value)) throw std::runtime_error("invalid direction");
  int previous_flag;
  if (!(std::cin >> previous_flag >> input.previous >> input.c1 >> input.c2 >> input.max_steps
        >> input.gradient_tol >> input.callback_grad_tol >> input.violation_tol
        >> input.sample_grad_squared)) throw std::runtime_error("missing control values");
  if ((previous_flag != 0 && previous_flag != 1) || input.max_steps <= 0)
    throw std::runtime_error("invalid flag or max_steps");
  if (!std::isfinite(input.p1) || !std::isfinite(input.p2) || input.p1 <= 0 || input.p2 <= 0 ||
      (previous_flag && !std::isfinite(input.previous)) || !std::isfinite(input.c1) ||
      !std::isfinite(input.c2) || !std::isfinite(input.gradient_tol) ||
      !std::isfinite(input.callback_grad_tol) || !std::isfinite(input.violation_tol) ||
      !std::isfinite(input.sample_grad_squared)) throw std::runtime_error("nonfinite control value");
  if (!(input.c1 > 0.0 && input.c1 < input.c2 && input.c2 < 1.0) ||
      input.gradient_tol < 0.0 || input.callback_grad_tol < 0.0 ||
      input.violation_tol < 0.0 || input.sample_grad_squared < 0.0)
    throw std::runtime_error("invalid tolerance or Wolfe constants");
  std::string extra;
  if (std::cin >> extra) throw std::runtime_error("unexpected trailing input");
  input.has_previous = previous_flag != 0;
  return input;
}

void jsonNumber(double value) {
  if (!std::isfinite(value)) throw std::runtime_error("nonfinite result");
  std::cout << std::setprecision(17) << value;
}

void jsonVector(const Vec& values) {
  std::cout << '[';
  for (size_t i = 0; i < values.size(); ++i) {
    if (i) std::cout << ',';
    jsonNumber(values[i]);
  }
  std::cout << ']';
}

double gradientSquared(const Vec& gradient) { return sqr(gradient); }

void printTrace(const std::vector<Vec>& states, const Vec& values, const Vec& norms,
                const std::vector<std::pair<double, double>>& weights) {
  std::cout << '[';
  for (size_t i = 0; i < states.size(); ++i) {
    if (i) std::cout << ',';
    std::cout << "{\"index\":" << i << ",\"value\":";
    jsonNumber(values[i]);
    std::cout << ",\"gradient_squared\":";
    jsonNumber(norms[i]);
    std::cout << ",\"weights\":[";
    jsonNumber(weights[i].first);
    std::cout << ',';
    jsonNumber(weights[i].second);
    std::cout << "],\"state\":";
    jsonVector(states[i]);
    std::cout << '}';
  }
  std::cout << ']';
}

int main(int argc, char** argv) {
  try {
    if (argc != 2) throw std::runtime_error("usage: reference_probe MODE");
    const std::string mode(argv[1]);
    if (mode != "objective" && mode != "line-search" && mode != "bfgs" &&
        mode != "adaptive" && mode != "schedule" && mode != "repair")
      throw std::runtime_error("unknown mode");
    ProbeInput input = readProbeInput();

    if (mode == "objective") {
      double result;
      Vec gradient(input.state.size());
      computeFunGrad(input.radii, input.state, input.p1, input.p2, result, gradient);
      double scalar;
      computeFun(input.radii, input.state, input.p1, input.p2, scalar);
      std::cout << "{\"value\":"; jsonNumber(result);
      std::cout << ",\"gradient\":"; jsonVector(gradient);
      std::cout << ",\"scalar\":"; jsonNumber(scalar); std::cout << "}\n";
      return 0;
    }

    if (mode == "line-search") {
      double step = std::numeric_limits<double>::quiet_NaN();
      double result = std::numeric_limits<double>::quiet_NaN();
      Fun f = [&](const Vec& x, double& value) { computeFun(input.radii, x, input.p1, input.p2, value); };
      FunGrad fg = [&](const Vec& x, double& value, Vec& gradient) {
        computeFunGrad(input.radii, x, input.p1, input.p2, value, gradient);
      };
      lineSearchStrongWolfe(makeSFun(f, input.state, input.direction),
                            makeSFunGrad(fg, input.state, input.direction),
                            input.has_previous ? input.previous : std::numeric_limits<double>::quiet_NaN(),
                            step, result, input.c1, input.c2, input.max_steps);
      if (!std::isfinite(step) || !std::isfinite(result)) throw std::runtime_error("line search exhausted");
      std::cout << "{\"step\":"; jsonNumber(step); std::cout << ",\"value\":";
      jsonNumber(result); std::cout << "}\n";
      return 0;
    }

    if (mode == "schedule") {
      double p1 = input.p1, p2 = input.p2;
      bool refresh = false, terminate = false;
      if (input.sample_grad_squared < input.callback_grad_tol) {
        terminate = true;
        Vec points(input.state.begin() + 1, input.state.end());
        double radius_real = computeValue(input.radii, {points, {0.0, 0.0}});
        double radius_ratio = radius_real / input.state[0];
        double min_scale = input.n == 1 ? 1.0 : computeMinScale(input.radii, points);
        double distance_ratio = 1.0 / min_scale;
        if (radius_ratio > 1.0 + input.violation_tol) { p1 *= 2.0; terminate = false; }
        if (distance_ratio > 1.0 + input.violation_tol) { p2 *= 2.0; terminate = false; }
        refresh = true;
      }
      std::cout << "{\"weights\":["; jsonNumber(p1); std::cout << ','; jsonNumber(p2);
      std::cout << "],\"refresh\":" << (refresh ? "true" : "false")
                << ",\"terminate\":" << (terminate ? "true" : "false") << "}\n";
      return 0;
    }

    if (mode == "repair") {
      Vec points(input.state.begin() + 1, input.state.end());
      double scale = input.n == 1 ? 1.0 : computeMinScale(input.radii, points);
      if (!(scale > 0.0) || !std::isfinite(scale)) throw std::runtime_error("cannot repair points");
      points = makeFeasible(input.radii, points, 1.0 + 5e-5);
      double radius = computeValue(input.radii, {points, {0.0, 0.0}});
      std::cout << "{\"radius\":"; jsonNumber(radius); std::cout << ",\"points\":";
      jsonVector(points); std::cout << ",\"min_scale\":"; jsonNumber(scale); std::cout << "}\n";
      return 0;
    }

    Vec x = input.state;
    double p1 = input.p1, p2 = input.p2;
    std::vector<Vec> trace_states;
    Vec trace_values, trace_norms;
    std::vector<std::pair<double, double>> trace_weights;
    auto f = [&](const Vec& state, double& result) { computeFun(input.radii, state, p1, p2, result); };
    auto fg = [&](const Vec& state, double& result, Vec& gradient) {
      computeFunGrad(input.radii, state, p1, p2, result, gradient);
    };
    bfgs(x, f, fg, 100000.0, input.max_steps, input.gradient_tol, input.c1, input.c2,
         [&](int, double, double value, double grad_squared) {
           trace_states.push_back(x);
           trace_values.push_back(value);
           trace_norms.push_back(grad_squared);
           trace_weights.emplace_back(p1, p2);
           Result action{};
           if (mode == "adaptive" && grad_squared < input.callback_grad_tol) {
             action.terminate = true;
             Vec points(x.begin() + 1, x.end());
             const double radius_real = computeValue(input.radii, {points, {0.0, 0.0}});
             const double radius_ratio = radius_real / x[0];
             const double min_scale = input.n == 1 ? 1.0 : computeMinScale(input.radii, points);
             const double distance_ratio = 1.0 / min_scale;
             if (radius_ratio > 1.0 + input.violation_tol) { p1 *= 2.0; action.terminate = false; }
             if (distance_ratio > 1.0 + input.violation_tol) { p2 *= 2.0; action.terminate = false; }
             action.update = true;
           }
           return action;
         });
    Vec final_gradient(x.size());
    double final_value;
    computeFunGrad(input.radii, x, p1, p2, final_value, final_gradient);
    std::cout << "{\"state\":"; jsonVector(x); std::cout << ",\"value\":";
    jsonNumber(final_value); std::cout << ",\"gradient\":"; jsonVector(final_gradient);
    std::cout << ",\"weights\":["; jsonNumber(p1); std::cout << ','; jsonNumber(p2);
    std::cout << "],\"trace\":";
    printTrace(trace_states, trace_values, trace_norms, trace_weights);
    std::cout << "}\n";
    return 0;
  } catch (const std::exception& error) {
    std::cerr << error.what() << '\n';
    return 2;
  }
}
